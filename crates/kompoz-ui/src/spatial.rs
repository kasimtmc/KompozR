//! Where layouts are in the window, as Compose's `androidx.compose.ui.spatial`: a layout's bounds relative to the
//! root and the window, how much of it shows and what is drawn over it ([`RelativeLayoutBounds`]), and callbacks
//! told when they change, throttled and debounced ([`NodeCx::register_on_layout_rect_changed`]).
//!
//! Compose keeps every layout's rectangle in its owner (the RectManager) and tells the callbacks after each frame;
//! here each callback's node is told where its layout went after each layout pass that moved it (the global
//! position callbacks), and keeps its own throttle and debounce on the frame clock — the same callbacks, at the
//! same times.

use crate::layout::{GlobalPositionAwareModifierNode, UnplacedAwareModifierNode};
use crate::modifier::{DelegatedNode, ModifierNode, NodeCx, NodeHandle, ObserverModifierNode};
use crate::node::LayoutNode;
use crate::node_capabilities;
use crate::platform::LocalWindowInfo;
use kompoz_coroutines::{Job, await_frame, delay_ms};
use kompoz_graphics::{IntOffset, IntRect, IntSize};
use std::cell::Cell;
use std::fmt;
use std::rc::{Rc, Weak};

/// A layout's bounds when it was last placed, relative to the root and the window, with the window's size (Compose's
/// RelativeLayoutBounds): what `on_layout_rect_changed` and `layout_bounds` give. In whole pixels. The host knows
/// no screen apart from its window: the screen's coordinates are the window's.
#[derive(Clone)]
pub struct RelativeLayoutBounds {
    top_left: IntOffset,
    bottom_right: IntOffset,
    /// Where the root is in the window.
    window_offset: IntOffset,
    window_size: IntSize,
    node: NodeHandle,
}

impl RelativeLayoutBounds {
    /// Its top left in the root (Compose's `positionInRoot`).
    pub fn position_in_root(&self) -> IntOffset {
        self.top_left
    }

    /// Its top left in the window (Compose's `positionInWindow`).
    pub fn position_in_window(&self) -> IntOffset {
        self.top_left + self.window_offset
    }

    /// Its top left on the screen (Compose's `positionInScreen`): in the window.
    pub fn position_in_screen(&self) -> IntOffset {
        self.position_in_window()
    }

    pub fn width(&self) -> i32 {
        self.bottom_right.x - self.top_left.x
    }

    pub fn height(&self) -> i32 {
        self.bottom_right.y - self.top_left.y
    }

    /// Its bounds in the root (Compose's `boundsInRoot`).
    pub fn bounds_in_root(&self) -> IntRect {
        IntRect::new(self.top_left.x, self.top_left.y, self.bottom_right.x, self.bottom_right.y)
    }

    /// Its bounds in the window (Compose's `boundsInWindow`).
    pub fn bounds_in_window(&self) -> IntRect {
        self.bounds_in_root().translate(self.window_offset)
    }

    /// Its bounds on the screen (Compose's `boundsInScreen`): in the window.
    pub fn bounds_in_screen(&self) -> IntRect {
        self.bounds_in_window()
    }

    /// The bounds in the root of the layouts drawn over it where they overlap it (Compose's
    /// `calculateOcclusions`): those drawn after it that aren't its ancestors or descendants, worked out now.
    pub fn calculate_occlusions(&self) -> Vec<IntRect> {
        let Some(target) = self.node.layout_node() else { return Vec::new() };
        let Some(root) = target.owner().and_then(|o| o.root()) else { return Vec::new() };
        if !target.is_placed() {
            return Vec::new();
        }
        let mine = layout_rect_in_root(&target);
        let mut out = Vec::new();
        walk_placed(&root, &mut |node| {
            if Rc::ptr_eq(node, &target) {
                return;
            }
            let rect = layout_rect_in_root(node);
            if intersects(mine, rect) && is_target_drawn_first(&target, node) {
                out.push(rect);
            }
        });
        out
    }

    /// How much of it is in `viewport`'s bounds, 0 to 1 (Compose's `fractionVisibleIn`).
    pub fn fraction_visible_in(&self, viewport: &RelativeLayoutBounds) -> f32 {
        let (tl, br) = (viewport.top_left, viewport.bottom_right);
        self.fraction_visible_in_rect(tl.x, tl.y, br.x, br.y)
    }

    /// How much of it is in the rectangle `left`, `top`, `right`, `bottom` of the root, 0 to 1: the area that
    /// is in it over the smaller of its area and the rectangle's (Compose's `fractionVisibleInRect`).
    pub fn fraction_visible_in_rect(&self, left: i32, top: i32, right: i32, bottom: i32) -> f32 {
        let (l, t, r, b) = (self.top_left.x, self.top_left.y, self.bottom_right.x, self.bottom_right.y);
        let clipped_left = l.max(left).min(right);
        let clipped_top = t.max(top).min(bottom);
        let clipped_right = r.min(right).max(left);
        let clipped_bottom = b.min(bottom).max(top);
        let viewport_area = (right - left) as i64 * (bottom - top) as i64;
        let rect_area = (r - l) as i64 * (b - t) as i64;
        let clipped_area = ((clipped_right - clipped_left) as i64 * (clipped_bottom - clipped_top) as i64).max(0);
        let max_area = viewport_area.min(rect_area);
        clipped_area as f32 / max_area as f32
    }

    /// How much of it is in the window, 0 to 1 (Compose's `fractionVisibleInWindow`).
    pub fn fraction_visible_in_window(&self) -> f32 {
        self.fraction_visible_in_rect(0, 0, self.window_size.width, self.window_size.height)
    }

    /// How much of it is in the window less `top_left_inset` and `bottom_right_inset` (the system bars, say), 0
    /// to 1 (Compose's `fractionVisibleInWindowWithInsets`).
    pub fn fraction_visible_in_window_with_insets(
        &self,
        top_left_inset: IntOffset,
        bottom_right_inset: IntOffset,
    ) -> f32 {
        let (x, y) = (self.window_offset.x, self.window_offset.y);
        self.fraction_visible_in_rect(
            x + top_left_inset.x,
            y + top_left_inset.y,
            x + self.window_size.width - bottom_right_inset.x,
            y + self.window_size.height - bottom_right_inset.y,
        )
    }
}

impl PartialEq for RelativeLayoutBounds {
    fn eq(&self, other: &Self) -> bool {
        self.top_left == other.top_left
            && self.bottom_right == other.bottom_right
            && self.window_offset == other.window_offset
            && self.window_size == other.window_size
            && Weak::ptr_eq(&self.node.cell, &other.node.cell)
    }
}

impl fmt::Debug for RelativeLayoutBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RelativeLayoutBounds(boundsInRoot={:?}, windowOffset={:?}, windowSize={:?})",
            self.bounds_in_root(),
            self.window_offset,
            self.window_size
        )
    }
}

/// A layout node's bounds in the root: its outer coordinates' box, in whole pixels (Compose's rect list entry).
fn layout_rect_in_root(node: &LayoutNode) -> IntRect {
    let outer = node.coordinates();
    outer.find_root_coordinates().local_bounding_box_of(&outer, false).round_to_int_rect()
}

/// Whether the two overlap, their right and bottom edges not included (Compose's `rectIntersectsRect`).
fn intersects(a: IntRect, b: IntRect) -> bool {
    a.right > b.left && a.bottom > b.top && b.right > a.left && b.bottom > a.top
}

/// Calls `f` on every placed node of the tree under `node`, `node` first.
fn walk_placed(node: &Rc<LayoutNode>, f: &mut dyn FnMut(&Rc<LayoutNode>)) {
    if !node.is_placed() {
        return;
    }
    f(node);
    for child in node.children() {
        walk_placed(&child, f);
    }
}

/// Whether `target` is drawn before `other` (Compose's `isTargetDrawnFirst`): false when either is the root or an
/// ancestor of the other; otherwise their branches under the nearest common ancestor decide, by z-index, then by
/// the order they were placed in.
fn is_target_drawn_first(target: &Rc<LayoutNode>, other: &Rc<LayoutNode>) -> bool {
    let (mut a, mut b) = (target.clone(), other.clone());
    if a.depth() == 0 || b.depth() == 0 {
        return false;
    }
    while a.depth() > b.depth() {
        let Some(p) = a.parent() else { return false };
        a = p;
    }
    if Rc::ptr_eq(&a, &b) {
        return false;
    }
    while b.depth() > a.depth() {
        let Some(p) = b.parent() else { return false };
        b = p;
    }
    if Rc::ptr_eq(&a, &b) {
        return false;
    }
    let (mut last_a, mut last_b) = (a.clone(), b.clone());
    while !Rc::ptr_eq(&a, &b) {
        last_a = a.clone();
        last_b = b.clone();
        let (Some(pa), Some(pb)) = (a.parent(), b.parent()) else { return false };
        a = pa;
        b = pb;
    }
    let (za, zb) = (last_a.z_index.get(), last_b.z_index.get());
    if za == zb { last_a.place_order.get() < last_b.place_order.get() } else { za < zb }
}

/// The bounds of the layout `handle`'s node is in, now: none while it isn't attached and placed.
pub(crate) fn relative_layout_bounds(handle: &NodeHandle) -> Option<RelativeLayoutBounds> {
    let node = handle.layout_node()?;
    if !node.is_attached() || !node.is_placed() {
        return None;
    }
    let owner = node.owner()?;
    let rect = layout_rect_in_root(&node);
    let window = owner.window_offset();
    Some(RelativeLayoutBounds {
        top_left: IntOffset::new(rect.left, rect.top),
        bottom_right: IntOffset::new(rect.right, rect.bottom),
        window_offset: IntOffset::new(window.x.round() as i32, window.y.round() as i32),
        window_size: node.current_value_of(&LocalWindowInfo).container_size.peek(),
        node: handle.clone(),
    })
}

/// Reads the window's size, so the reading scope is told when it changes (what the bounds' window fraction
/// depends on beside the layout's place).
pub(crate) fn read_window_size(cx: &NodeCx) {
    cx.current_value_of(&LocalWindowInfo).container_size();
}

/// The frame clock's time, in milliseconds.
pub(crate) fn now_millis(handle: &NodeHandle) -> i64 {
    handle.layout_node().and_then(|n| n.owner()).map_or(0, |o| o.dispatcher().now() / 1_000_000)
}

// ---------------------------------------------------------------------------------------------------------------
// Rect changed callbacks

/// A rect changed callback.
pub(crate) type RectCallback = Rc<dyn Fn(&RelativeLayoutBounds)>;

/// What [`NodeCx::register_on_layout_rect_changed`] gives (Compose's RegistrationHandle): unregister the callback
/// with it.
pub struct RegistrationHandle {
    delegator: NodeHandle,
    delegate: DelegatedNode<OnLayoutRectChangedNode>,
    alive: Rc<Cell<bool>>,
}

impl RegistrationHandle {
    /// The callback is told nothing more; its node is taken off (at once, or, from inside its own callback, at the
    /// next frame).
    pub fn unregister(self) {
        self.alive.set(false);
        let Some(cx) = self.delegator.cx() else { return };
        if self.delegate.cell.node.try_borrow_mut().is_ok() {
            cx.undelegate(&self.delegate);
        } else if cx.is_attached() {
            let (delegator, delegate) = (self.delegator.clone(), self.delegate.clone());
            cx.coroutine_scope().launch(async move {
                await_frame().await;
                if let Some(cx) = delegator.cx() {
                    cx.undelegate(&delegate);
                }
            });
        }
    }
}

impl NodeCx {
    /// Calls `callback` with the bounds of the layout this node is in whenever they change, Compose's way
    /// (Compose's `DelegatableNode.registerOnLayoutRectChanged`): at once when the last call is `throttle_millis`
    /// or more ago, else once they have been still for `debounce_millis` (the throttle's when 0); with both 0, at
    /// once each time. Told after each layout pass, and when the window resizes.
    pub fn register_on_layout_rect_changed(
        &self,
        throttle_millis: i64,
        debounce_millis: i64,
        callback: impl Fn(&RelativeLayoutBounds) + 'static,
    ) -> RegistrationHandle {
        let node = OnLayoutRectChangedNode::new(throttle_millis, debounce_millis, Rc::new(callback));
        let alive = node.alive.clone();
        let delegate = self.delegate(node);
        RegistrationHandle { delegator: self.handle(), delegate, alive }
    }
}

/// The node of `on_layout_rect_changed`, and the one a registration delegates to (Compose's
/// OnLayoutRectChangedNode with its ThrottledCallbacks entry): told where its layout is after each layout pass,
/// it calls back as its throttle and debounce say.
pub(crate) struct OnLayoutRectChangedNode {
    throttle_millis: i64,
    /// The debounce it goes by: the throttle's when asked for none, so the last place is told.
    debounce_millis: i64,
    callback: RectCallback,
    /// False once unregistered.
    alive: Rc<Cell<bool>>,
    handle: Option<NodeHandle>,
    /// The last bounds seen.
    latest: Option<RelativeLayoutBounds>,
    last_invoke_millis: Option<i64>,
    /// When a change wasn't told at once (for the debounce).
    last_uninvoked_fire_millis: Option<i64>,
    timer: Option<Job>,
}

impl OnLayoutRectChangedNode {
    pub(crate) fn new(throttle_millis: i64, debounce_millis: i64, callback: RectCallback) -> OnLayoutRectChangedNode {
        OnLayoutRectChangedNode {
            throttle_millis,
            debounce_millis: if debounce_millis == 0 { throttle_millis } else { debounce_millis },
            callback,
            alive: Rc::new(Cell::new(true)),
            handle: None,
            latest: None,
            last_invoke_millis: None,
            last_uninvoked_fire_millis: None,
            timer: None,
        }
    }

    /// New settings (an element's update): told as a new registration (Compose's `disposeAndRegister`).
    pub(crate) fn update(&mut self, throttle_millis: i64, debounce_millis: i64, callback: RectCallback) {
        self.throttle_millis = throttle_millis;
        self.debounce_millis = if debounce_millis == 0 { throttle_millis } else { debounce_millis };
        self.callback = callback;
        self.last_invoke_millis = None;
        self.last_uninvoked_fire_millis = None;
        if let Some(timer) = self.timer.take() {
            timer.cancel();
        }
    }

    /// Where its layout is now, told if it moved.
    fn check(&mut self) {
        let Some(handle) = self.handle.clone() else { return };
        let Some(bounds) = relative_layout_bounds(&handle) else { return };
        if self.latest.as_ref() == Some(&bounds) {
            return;
        }
        self.latest = Some(bounds);
        self.fire_with_updated_rect(now_millis(&handle));
    }

    /// Compose's `fireWithUpdatedRect`: told at once past the throttle (when it may be at once), else the debounce
    /// waits.
    fn fire_with_updated_rect(&mut self, now: i64) {
        let past_throttle = self.last_invoke_millis.is_none_or(|last| now - last >= self.throttle_millis);
        let zero_debounce = self.debounce_millis == 0;
        let zero_throttle = self.throttle_millis == 0;
        let can_invoke = (!zero_debounce && !zero_throttle) || zero_debounce;
        if past_throttle && can_invoke {
            self.last_uninvoked_fire_millis = None;
            self.last_invoke_millis = Some(now);
            if let Some(timer) = self.timer.take() {
                timer.cancel();
            }
            self.fire();
        } else if !zero_debounce {
            self.last_uninvoked_fire_millis = Some(now);
            self.schedule_debounce(self.debounce_millis);
        }
    }

    fn fire(&self) {
        if self.alive.get()
            && let Some(bounds) = &self.latest
        {
            (self.callback)(bounds);
        }
    }

    /// Wakes after `millis` to tell what the debounce held back.
    fn schedule_debounce(&mut self, millis: i64) {
        if let Some(timer) = self.timer.take() {
            timer.cancel();
        }
        let Some(handle) = self.handle.clone() else { return };
        let Some(cx) = handle.cx().filter(|cx| cx.is_attached()) else { return };
        self.timer = Some(cx.coroutine_scope().launch(async move {
            delay_ms(millis.max(0) as u64).await;
            let Some(cx) = handle.cx() else { return };
            let now = now_millis(&handle);
            // (the callback is called with the node let go)
            let due = cx.with::<OnLayoutRectChangedNode, _>(|n| n.take_debounced(now)).flatten();
            if let Some((callback, bounds)) = due {
                callback(&bounds);
            }
        }));
    }

    /// Compose's `debounceEntry`: what the debounce held back, if it is due.
    fn take_debounced(&mut self, now: i64) -> Option<(RectCallback, RelativeLayoutBounds)> {
        self.timer = None;
        let last = self.last_uninvoked_fire_millis?;
        if self.debounce_millis <= 0 {
            return None;
        }
        if now - last < self.debounce_millis {
            self.schedule_debounce(last + self.debounce_millis - now);
            return None;
        }
        self.last_invoke_millis = Some(now);
        self.last_uninvoked_fire_millis = None;
        if !self.alive.get() {
            return None;
        }
        Some((self.callback.clone(), self.latest.clone()?))
    }

    /// Reads the window's size, observed.
    fn observe_window(&self) {
        if let Some(cx) = self.handle.as_ref().and_then(|h| h.cx()) {
            cx.observe_reads(|| read_window_size(&cx));
        }
    }
}

impl ModifierNode for OnLayoutRectChangedNode {
    node_capabilities!(global_position_aware, unplaced_aware, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.observe_window();
    }

    fn on_detach(&mut self, _: &NodeCx) {
        // (its coroutines, the debounce's timer, go with it)
        self.timer = None;
        self.handle = None;
    }

    fn on_reset(&mut self) {
        self.latest = None;
        self.last_invoke_millis = None;
        self.last_uninvoked_fire_millis = None;
    }
}

impl GlobalPositionAwareModifierNode for OnLayoutRectChangedNode {
    fn on_globally_positioned(&mut self, _: &crate::layout::LayoutCoordinates) {
        self.check();
    }
}

impl UnplacedAwareModifierNode for OnLayoutRectChangedNode {
    fn on_unplaced(&mut self) {
        // (placed again, it is told where, as a layout put back in Compose's rect list is)
        self.latest = None;
    }
}

impl ObserverModifierNode for OnLayoutRectChangedNode {
    fn on_observed_reads_changed(&mut self) {
        self.observe_window();
        self.check();
    }
}

//! What a lazy layout's state uses besides (Compose's ObservableScopeInvalidator.kt, AwaitFirstLayoutModifier.kt,
//! the RemeasurementModifier it gets its node through, LazyLayoutBringIntoViewSpec.kt): a state read to be
//! measured or placed again, waiting for the first layout, measuring the layout again now, and bringing into view
//! under sticky headers.

use crate::gestures::{BringIntoViewSpec, LocalBringIntoViewSpec};
use kompoz_graphics::LayoutDirection;
use kompoz_runtime::{MutableState, kompozable, mutable_state_of_with_policy, never_equal_policy, remember_keyed};
use kompoz_ui::layout::{LayoutAwareModifierNode, LayoutCoordinates};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// A state read only to be told (Compose's ObservableScopeInvalidator): a measure or placement reading it runs
/// again when it is invalidated.
#[derive(Clone)]
pub(crate) struct ObservableScopeInvalidator(MutableState<()>);

impl Default for ObservableScopeInvalidator {
    fn default() -> Self {
        ObservableScopeInvalidator(mutable_state_of_with_policy((), never_equal_policy()))
    }
}

impl ObservableScopeInvalidator {
    pub(crate) fn attach_to_scope(&self) {
        self.0.get();
    }

    pub(crate) fn invalidate_scope(&self) {
        self.0.set(());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Measuring the layout again now

/// The layout node a modifier is on, to measure it again at once (Compose's Remeasurement, which a
/// RemeasurementModifier is given). Clones are the same.
#[derive(Clone, Default)]
pub(crate) struct Remeasurement(Rc<RefCell<Option<NodeHandle>>>);

impl PartialEq for Remeasurement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Remeasurement {
    /// Measures and places the layout now (Compose's `forceRemeasure`), if it is there.
    pub(crate) fn force_remeasure(&self) {
        let handle = self.0.borrow().clone();
        if let Some(h) = handle {
            h.remeasure_sync();
        }
    }

    /// The modifier telling it its layout (Compose's RemeasurementModifier).
    pub(crate) fn modifier(&self) -> Modifier {
        Modifier.then_element(RemeasurementElement(self.clone()))
    }
}

#[derive(PartialEq)]
struct RemeasurementElement(Remeasurement);

impl ModifierNodeElement for RemeasurementElement {
    type Node = RemeasurementNode;
    fn create(&self) -> RemeasurementNode {
        RemeasurementNode(self.0.clone())
    }
    fn update(&self, node: &mut RemeasurementNode) {
        node.0 = self.0.clone();
    }
    fn name(&self) -> &'static str {
        "remeasurementModifier"
    }
}

struct RemeasurementNode(Remeasurement);

impl ModifierNode for RemeasurementNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        *self.0.0.borrow_mut() = Some(cx.handle());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Waiting for the first layout

/// What a scroll waits on until the layout was laid out once (Compose's AwaitFirstLayoutModifier). Clones are the
/// same.
#[derive(Clone, Default)]
pub(crate) struct AwaitFirstLayoutModifier(Rc<AwaitInner>);

#[derive(Default)]
struct AwaitInner {
    /// Waiting: whether it was laid out, and who waits.
    lock: RefCell<Option<Rc<RefCell<Lock>>>>,
}

#[derive(Default)]
struct Lock {
    done: bool,
    waiters: Vec<Waker>,
}

impl PartialEq for AwaitFirstLayoutModifier {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl AwaitFirstLayoutModifier {
    /// Waits until the layout is laid out next (Compose's `waitForFirstLayout`).
    pub(crate) fn wait_for_first_layout(&self) -> impl Future<Output = ()> + 'static {
        let lock = self.0.lock.borrow_mut().get_or_insert_with(Default::default).clone();
        LockFuture(lock)
    }

    pub(crate) fn modifier(&self) -> Modifier {
        Modifier.then_element(AwaitFirstLayoutElement(self.clone()))
    }

    /// Laid out: who waits goes on.
    fn complete(&self) {
        let Some(lock) = self.0.lock.borrow_mut().take() else { return };
        let waiters = {
            let mut l = lock.borrow_mut();
            l.done = true;
            std::mem::take(&mut l.waiters)
        };
        for w in waiters {
            w.wake();
        }
    }
}

struct LockFuture(Rc<RefCell<Lock>>);

impl Future for LockFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut l = self.0.borrow_mut();
        if l.done {
            Poll::Ready(())
        } else {
            l.waiters.push(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[derive(PartialEq)]
struct AwaitFirstLayoutElement(AwaitFirstLayoutModifier);

impl ModifierNodeElement for AwaitFirstLayoutElement {
    type Node = AwaitFirstLayoutNode;
    fn create(&self) -> AwaitFirstLayoutNode {
        AwaitFirstLayoutNode(self.0.clone())
    }
    fn update(&self, _: &mut AwaitFirstLayoutNode) {}
    fn name(&self) -> &'static str {
        "AwaitFirstLayoutModifier"
    }
}

/// Compose's `AwaitFirstLayoutModifier.Node`: placed, it lets the waiters go (Compose registers a layout rect
/// callback for that).
struct AwaitFirstLayoutNode(AwaitFirstLayoutModifier);

impl ModifierNode for AwaitFirstLayoutNode {
    node_capabilities!(layout_aware);
}

impl LayoutAwareModifierNode for AwaitFirstLayoutNode {
    fn on_placed(&mut self, _: &LayoutCoordinates) {
        self.0.complete();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Bringing into view under sticky items

/// The bring-into-view spec of a lazy layout with sticky items (Compose's `rememberLazyLayoutBringIntoViewSpec`):
/// the local one, on the part of the viewport the sticking items leave.
#[kompozable]
pub(crate) fn remember_lazy_layout_bring_into_view_spec(
    reverse_layout: bool,
    is_vertical: bool,
    sticky_items_combined_size_lambda: Rc<dyn Fn() -> i32>,
) -> Rc<dyn BringIntoViewSpec> {
    let current_bring_into_view_spec = LocalBringIntoViewSpec.current();
    let layout_direction = LocalLayoutDirection.current();
    let keys = (
        Rc::as_ptr(&sticky_items_combined_size_lambda) as *const () as usize,
        reverse_layout,
        layout_direction,
        SpecKey(current_bring_into_view_spec.clone()),
        is_vertical,
    );
    remember_keyed(keys, move || {
        Rc::new(StickyHeaderBringIntoViewSpec {
            sticky_items_combined_size_lambda,
            reverse_layout,
            layout_direction,
            is_vertical,
            bring_into_view_spec: current_bring_into_view_spec,
        }) as Rc<dyn BringIntoViewSpec>
    })
}

/// A spec compared as specs are.
#[derive(Clone)]
struct SpecKey(Rc<dyn BringIntoViewSpec>);

impl PartialEq for SpecKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.0.bring_into_view_spec_eq(other.0.as_any())
    }
}

/// Compose's StickyHeaderBringIntoViewSpec.
struct StickyHeaderBringIntoViewSpec {
    sticky_items_combined_size_lambda: Rc<dyn Fn() -> i32>,
    reverse_layout: bool,
    layout_direction: LayoutDirection,
    is_vertical: bool,
    bring_into_view_spec: Rc<dyn BringIntoViewSpec>,
}

impl PartialEq for StickyHeaderBringIntoViewSpec {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl BringIntoViewSpec for StickyHeaderBringIntoViewSpec {
    fn calculate_scroll_distance(&self, offset: f32, size: f32, container_size: f32) -> f32 {
        let non_sticky_items_offset = (self.sticky_items_combined_size_lambda)() as f32;
        // (horizontal not reversed left to right, or reversed right to left, starts at the left: as a column)
        let is_rtl = self.layout_direction == LayoutDirection::Rtl;
        let apply_sticky_offset = self.reverse_layout == (!self.is_vertical && is_rtl);
        let valid_offset = if apply_sticky_offset { offset - non_sticky_items_offset } else { offset };
        self.bring_into_view_spec.calculate_scroll_distance(
            valid_offset,
            size,
            container_size - non_sticky_items_offset,
        )
    }
}

//! Resizing a pane by dragging its edge, as Compose's internal (unpublished) DragToResizeState.kt (adaptive-layout
//! 1.2.0): a draggable state of a size docked to an edge ([`DragToResizeState`], [`remember_drag_to_resize_state`],
//! [`DockedEdge`]) and the click that expands, collapses or partly expands it ([`click_to_resize`]).

// (Compose keeps it internal and unpublished: no scaffold of adaptive-layout 1.2.0 uses it yet; tested by the unit
// tests below)
#![allow(dead_code)]

use super::internal::delegable_modifier::{DelegableSemanticsPropertyReceiver, delegable_semantics};
use super::internal::strings::{Strings, get_string_of_node};
use kompoz_animation::{MutatePriority, MutatorMutex, animate};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::gestures::{DragBlock, DragScope, DraggableState, LocalFuture, Orientation};
use kompoz_foundation::prelude::ClickableModifiers;
use kompoz_graphics::{Dp, LayoutDirection};
use kompoz_runtime::{
    MutableState, Saveable, SavedValue, Saver, list_saver, mutable_float_state_of, mutable_state_of, named_args,
    remember_saveable_keyed_with, without_read_observation,
};
use kompoz_ui::Modifier;
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use std::cell::{Cell, RefCell};
use std::ops::RangeInclusive;
use std::rc::Rc;

/// The edge a resizable pane is docked to (Compose's internal enum `DockedEdge`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum DockedEdge {
    Top,
    Bottom,
    Start,
    End,
}

/// A drag-to-resize state remembered (and saved) for `docked_edge` (Compose's internal composable
/// `rememberDragToResizeState`): within `min_size` (unspecified: the scaffold's default minimum) and `max_size`
/// (unspecified: no maximum) in this density; the composition's coroutine scope animating it.
#[track_caller]
#[allow(dead_code)]
pub(crate) fn remember_drag_to_resize_state(docked_edge: DockedEdge, min_size: Dp, max_size: Dp) -> DragToResizeState {
    kompoz_runtime::kompozable_block!({
        let layout_direction = LocalLayoutDirection.current();
        let density = LocalDensity.current();
        let coroutine_scope = remember_coroutine_scope();
        let state =
            remember_saveable_keyed_with(docked_edge, &DragToResizeState::Saver(docked_edge, layout_direction), || {
                DragToResizeState::of_edge(docked_edge, layout_direction)
            });
        *state.inner.coroutine_scope.borrow_mut() = Some(coroutine_scope);
        state.inner.min_size.set(if min_size.is_specified() { density.round_to_px(min_size) } else { i32::MIN });
        state.inner.max_size.set(if max_size.is_specified() { density.round_to_px(max_size) } else { i32::MAX });
        state
    })
}

/// Which edge a state's size grows from (Compose's internal subclasses `Left`, `Right`, `Top`, `Bottom`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DragToResizeKind {
    Left,
    Right,
    Top,
    Bottom,
}

/// Where a resizable pane's size is (Compose's internal enum `DragToResizeState.State`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum DragToResizeValue {
    Expanded,
    Collapsed,
    Dragged,
    Default,
}

impl DragToResizeValue {
    const ENTRIES: [DragToResizeValue; 4] = [
        DragToResizeValue::Expanded,
        DragToResizeValue::Collapsed,
        DragToResizeValue::Dragged,
        DragToResizeValue::Default,
    ];
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl Saveable for DragToResizeValue {
    fn save(&self) -> SavedValue {
        SavedValue::Int(*self as i64)
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        DragToResizeValue::ENTRIES.get(ordinal).copied()
    }
}

/// The size of a pane dragged to resize from the edge it is docked to (Compose's internal abstract class
/// `DragToResizeState`, a `DraggableState`): its size within its range (at an end it is collapsed or expanded), its
/// state, the size a measurement starts it at. A cheap handle: its clones are the same state, equal only to
/// themselves.
#[derive(Clone)]
pub(crate) struct DragToResizeState {
    inner: Rc<Inner>,
}

struct Inner {
    kind: DragToResizeKind,
    // TODO(conradchen): To figure out a better way to expose this in the relevant APIs
    coroutine_scope: RefCell<Option<CoroutineScope>>,
    size_internal: MutableState<f32>,
    state: MutableState<DragToResizeValue>,
    width_range: RefCell<RangeInclusive<f32>>,
    height_range: RefCell<RangeInclusive<f32>>,
    min_size: Cell<i32>,
    max_size: Cell<i32>,
    last_measured_size: Cell<f32>,
    last_dragged_size: Cell<f32>,
    drag_mutex: MutatorMutex,
}

impl PartialEq for DragToResizeState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

#[allow(dead_code)]
impl DragToResizeState {
    fn new(kind: DragToResizeKind) -> DragToResizeState {
        DragToResizeState {
            inner: Rc::new(Inner {
                kind,
                coroutine_scope: RefCell::new(None),
                size_internal: mutable_float_state_of(f32::NAN),
                state: mutable_state_of(DragToResizeValue::Default),
                width_range: RefCell::new(0.0..=0.0),
                height_range: RefCell::new(0.0..=0.0),
                min_size: Cell::new(0),
                max_size: Cell::new(0),
                last_measured_size: Cell::new(f32::NAN),
                last_dragged_size: Cell::new(f32::NAN),
                drag_mutex: MutatorMutex::new(),
            }),
        }
    }

    /// The state of an edge in a layout direction (Compose's private factory `DragToResizeState(dockedEdge,
    /// layoutDirection)`): the start is the left left to right, the right right to left.
    pub(crate) fn of_edge(docked_edge: DockedEdge, layout_direction: LayoutDirection) -> DragToResizeState {
        let ltr = layout_direction == LayoutDirection::Ltr;
        DragToResizeState::new(match docked_edge {
            DockedEdge::Top => DragToResizeKind::Top,
            DockedEdge::Bottom => DragToResizeKind::Bottom,
            DockedEdge::Start if ltr => DragToResizeKind::Left,
            DockedEdge::Start => DragToResizeKind::Right,
            DockedEdge::End if ltr => DragToResizeKind::Right,
            DockedEdge::End => DragToResizeKind::Left,
        })
    }

    pub(crate) fn kind(&self) -> DragToResizeKind {
        self.inner.kind
    }

    fn is_horizontal(&self) -> bool {
        matches!(self.inner.kind, DragToResizeKind::Left | DragToResizeKind::Right)
    }

    /// The axis it is dragged along (Compose's `orientation`).
    pub(crate) fn orientation(&self) -> Orientation {
        if self.is_horizontal() { Orientation::Horizontal } else { Orientation::Vertical }
    }

    /// The size (Compose's internal `size`): NaN until measured.
    pub(crate) fn size(&self) -> f32 {
        self.inner.size_internal.get()
    }

    /// (Compose's private setter of `size`: within the range, at or past an end collapsed or expanded.)
    fn set_size(&self, value: f32) {
        let range = self.size_range();
        let size = if range.start() > range.end() {
            value
        } else if value <= *range.start() {
            self.inner.state.set(DragToResizeValue::Collapsed);
            *range.start()
        } else if value >= *range.end() {
            self.inner.state.set(DragToResizeValue::Expanded);
            *range.end()
        } else {
            value
        };
        self.inner.size_internal.set(size);
    }

    /// Its state (Compose's internal `state`).
    pub(crate) fn state(&self) -> DragToResizeValue {
        self.inner.state.get()
    }

    /// The range of its size: the width's or the height's (Compose's `sizeRange`).
    pub(crate) fn size_range(&self) -> RangeInclusive<f32> {
        if self.is_horizontal() {
            self.inner.width_range.borrow().clone()
        } else {
            self.inner.height_range.borrow().clone()
        }
    }

    /// A drag's delta along its size: the opposite for a size growing from the right or the bottom (Compose's
    /// `convertDelta`).
    fn convert_delta(&self, delta: f32) -> f32 {
        match self.inner.kind {
            DragToResizeKind::Right | DragToResizeKind::Bottom => -delta,
            _ => delta,
        }
    }

    /// The width to measure the pane at (Compose's internal `getDraggedWidth`): a horizontal state's dragged size,
    /// its range from its minimum (else `default_min_width`) to the smaller of its maximum and the scaffold's width;
    /// a vertical one's `measuring_width`.
    pub(crate) fn get_dragged_width(&self, measuring_width: i32, default_min_width: i32, scaffold_width: i32) -> i32 {
        if !self.is_horizontal() {
            return measuring_width;
        }
        let min_size = self.inner.min_size.get();
        let min_width = if min_size == i32::MIN { default_min_width } else { min_size };
        *self.inner.width_range.borrow_mut() = to_float_range(min_width, self.inner.max_size.get().min(scaffold_width));
        self.get_and_update_dragged_size(measuring_width)
    }

    /// The height to measure the pane at (Compose's internal `getDraggedHeight`): as the width's, for a vertical
    /// state.
    pub(crate) fn get_dragged_height(
        &self,
        measuring_height: i32,
        default_min_height: i32,
        scaffold_height: i32,
    ) -> i32 {
        if self.is_horizontal() {
            return measuring_height;
        }
        let min_size = self.inner.min_size.get();
        let min_height = if min_size == i32::MIN { default_min_height } else { min_size };
        *self.inner.height_range.borrow_mut() =
            to_float_range(min_height, self.inner.max_size.get().min(scaffold_height));
        self.get_and_update_dragged_size(measuring_height)
    }

    /// The size a measurement of `measuring_size` gets (Compose's internal `getAndUpdateDraggedSize`): the dragged
    /// size, coerced into the range again, or, before one, the size of its state.
    pub(crate) fn get_and_update_dragged_size(&self, measuring_size: i32) -> i32 {
        self.inner.last_measured_size.set(measuring_size as f32);
        let size = self.size();
        // Note that we still need to set it again in this case to re-coerce the value
        let size = if size.is_nan() { self.get_target_size(self.state()) } else { size };
        self.set_size(size);
        self.size() as i32
    }

    /// The size the drag left last, else the size measured last (Compose's private `defaultSize`).
    fn default_size(&self) -> f32 {
        let last_dragged = self.inner.last_dragged_size.get();
        if last_dragged.is_nan() { self.inner.last_measured_size.get() } else { last_dragged }
    }

    /// The size of a state (Compose's private `getTargetSize`).
    fn get_target_size(&self, target_state: DragToResizeValue) -> f32 {
        match target_state {
            DragToResizeValue::Expanded => *self.size_range().end(),
            DragToResizeValue::Collapsed => *self.size_range().start(),
            DragToResizeValue::Default => self.default_size(),
            DragToResizeValue::Dragged => self.size(),
        }
    }

    /// Goes to `target_state` (Compose's internal `animateTo(targetState)`): from the default size at once, else
    /// animated in the remembered coroutine scope.
    pub(crate) fn animate_to_state(&self, target_state: DragToResizeValue) {
        if self.state() == target_state {
            return;
        }
        self.animate_to_size(self.get_target_size(target_state));
        self.inner.state.set(target_state);
    }

    /// (Compose's private `animateTo(targetSize)`.)
    fn animate_to_size(&self, target_size: f32) {
        if self.state() == DragToResizeValue::Default {
            self.set_size(target_size);
        } else {
            let scope = self.inner.coroutine_scope.borrow().clone().expect("the remembered coroutine scope");
            let this = self.clone();
            let animation =
                named_args!(animate(this.size(), target_size, block = move |value, _| this.set_size(value)));
            named_args!(scope.launch(animation));
        }
    }

    /// The state after this one a click goes to (Compose's private `nextState`): expanded to collapsed, collapsed
    /// to the default size, else expanded.
    pub(crate) fn next_state(&self) -> DragToResizeValue {
        match self.state() {
            DragToResizeValue::Expanded => DragToResizeValue::Collapsed,
            DragToResizeValue::Collapsed => DragToResizeValue::Default,
            _ => DragToResizeValue::Expanded,
        }
    }

    /// Saved as `[size, state, width range's start and end, height range's start and end]` (Compose's internal
    /// companion's `Saver(dockedEdge, layoutDirection)`), restored as a state of that edge.
    pub(crate) fn Saver(docked_edge: DockedEdge, layout_direction: LayoutDirection) -> Saver<DragToResizeState> {
        list_saver(
            |s: &DragToResizeState| {
                let (w, h) = (s.inner.width_range.borrow().clone(), s.inner.height_range.borrow().clone());
                vec![
                    SavedValue::Double(s.inner.size_internal.peek() as f64),
                    s.inner.state.peek().save(),
                    SavedValue::Double(*w.start() as f64),
                    SavedValue::Double(*w.end() as f64),
                    SavedValue::Double(*h.start() as f64),
                    SavedValue::Double(*h.end() as f64),
                ]
            },
            move |items: &[SavedValue]| {
                let float = |k: usize| match items.get(k)? {
                    SavedValue::Double(d) => Some(*d as f32),
                    _ => None,
                };
                let state = DragToResizeState::of_edge(docked_edge, layout_direction);
                state.set_size(float(0)?);
                state.inner.state.set(DragToResizeValue::restore(items.get(1)?)?);
                *state.inner.width_range.borrow_mut() = float(2)?..=float(3)?;
                *state.inner.height_range.borrow_mut() = float(4)?..=float(5)?;
                Some(state)
            },
        )
    }
}

impl DraggableState for DragToResizeState {
    fn drag_block(&self, drag_priority: MutatePriority, block: DragBlock) -> LocalFuture<'_, ()> {
        let this = self.clone();
        Box::pin(async move {
            let scope: Rc<dyn DragScope> = Rc::new(ResizeDragScope(this.clone()));
            named_args!(this.inner.drag_mutex.mutate_with(scope, priority = drag_priority, block = block)).await;
        })
    }

    fn dispatch_raw_delta(&self, delta: f32) {
        if without_read_observation(|| self.size().is_nan()) {
            return;
        }
        let actual_delta = self.convert_delta(delta);
        self.inner.state.set(DragToResizeValue::Dragged);
        self.set_size(self.size() + actual_delta);
        self.inner.last_dragged_size.set(self.size());
    }
}

/// The state's drag scope (Compose's private `dragScope`).
struct ResizeDragScope(DragToResizeState);

impl DragScope for ResizeDragScope {
    fn drag_by(&self, pixels: f32) {
        self.0.dispatch_raw_delta(pixels);
    }
}

/// Kotlin's private `IntRange.toFloatRange()`.
fn to_float_range(first: i32, last: i32) -> RangeInclusive<f32> {
    first as f32..=last as f32
}

/// `modifier` clicked to the state's next state, and telling accessibility the click's label and the state
/// (Compose's internal `Modifier.clickToResize`).
// TODO(conradchen): Figure out if we should publish it and the right API shape
#[allow(dead_code)]
pub(crate) fn click_to_resize(modifier: Modifier, state: DragToResizeState) -> Modifier {
    let clicked = state.clone();
    let modifier = named_args!(modifier.clickable(on_click = move || clicked.animate_to_state(clicked.next_state())));
    delegable_semantics(
        modifier,
        false,
        Rc::new(move |r: &mut DelegableSemanticsPropertyReceiver| {
            let node = r.node().clone();
            let next_state = state.next_state();
            let label = get_string_of_node(
                &node,
                match next_state {
                    DragToResizeValue::Expanded => Strings::dragToResizeClickToExpandDescription,
                    DragToResizeValue::Collapsed => Strings::dragToResizeClickToCollapseDescription,
                    _ => Strings::dragToResizeClickToPartiallyExpandDescription,
                },
            );
            let clicked = state.clone();
            named_args!(r.on_click(
                label = Some(label),
                action = move || {
                    clicked.animate_to_state(next_state);
                    true
                }
            ));
            let state_description = get_string_of_node(
                &node,
                match state.state() {
                    DragToResizeValue::Expanded => Strings::dragToResizeExpandedStateDescription,
                    DragToResizeValue::Collapsed => Strings::dragToResizeCollapsedStateDescription,
                    _ => Strings::dragToResizePartiallyExpandedStateDescription,
                },
            );
            r.set_state_description(state_description);
        }),
    )
}
